//! Planche des grandes boîtes de dialogue : chaque boîte (et chaque page des Options, chaque onglet) ouverte dans
//! l'application sans écran, dessinée dans un PNG.
//!
//! `cargo run -p alpirc-shell --example dialogs -- /tmp/dialogs [fr] [dark] [largeur hauteur]`
//! écrit `/tmp/dialogs/<nom>.png`.

use std::path::Path;

use alpirc_core::UiDialog;
use alpirc_core::settings::{ControlEntry, ControlKind, IgnoreEntry, Network, NotifyEntry, Server};
use alpirc_net::{Net, NetEvent};
use alpirc_shell::{LayoutKind, Shell, ShellConfig, ShellPlatform};
use alpirc_ui::{Canvas, Cursor, Input, Key, Mods, Platform, Rect, Size};

struct P {
    clip: Option<String>,
    now: i64,
}

impl Platform for P {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clip.clone()
    }
    fn clipboard_set(&mut self, text: &str) {
        self.clip = Some(text.into());
    }
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
}

fn shot(shell: &mut Shell, dir: &Path, name: &str) {
    let vp = shell.viewport();
    let mut canvas = Canvas::new(vp.w as u32, vp.h as u32, 1.0);
    shell.paint(&mut canvas);
    let path = dir.join(format!("{name}.png"));
    canvas.to_png(&path).unwrap();
    println!("{}", path.display());
}

fn key(shell: &mut Shell, p: &mut P, k: Key, mods: Mods) {
    shell.input(Input::key(k, mods), p);
}

fn close(shell: &mut Shell, p: &mut P) {
    key(shell, p, Key::Escape, Mods::NONE);
}

/// Photographie la boîte ouverte, puis chacun de ses onglets suivants (Ctrl+Tab).
fn shots_with_tabs(shell: &mut Shell, p: &mut P, dir: &Path, name: &str, tabs: &[&str]) {
    shot(shell, dir, &format!("{name}-{}", tabs[0]));
    for t in &tabs[1..] {
        key(shell, p, Key::Tab, Mods::CTRL);
        shot(shell, dir, &format!("{name}-{t}"));
    }
    close(shell, p);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "/tmp/dialogs".into()));
    let fr = args.iter().any(|a| a == "fr");
    let dark = args.iter().any(|a| a == "dark");
    let nums: Vec<i32> = args.iter().skip(2).filter_map(|a| a.parse().ok()).collect();
    let (w, h) = if nums.len() >= 2 { (nums[0], nums[1]) } else { (800, 600) };
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::env::temp_dir().join(format!("alpirc-dialogs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = P { clip: None, now: 1_792_260_000_000 };
    let net = Net::new(Box::new(|_e: NetEvent| {}));
    let cfg = ShellConfig {
        settings_path: dir.join("settings.toml"),
        data_dir: dir.clone(),
        layout: LayoutKind::Desktop,
        utc_offset: 7200,
        locale: Some(if fr { "fr-FR" } else { "en-US" }.into()),
        now_ms: p.now,
    };
    let mut shell = Shell::new(cfg, net);
    {
        let s = shell.client.settings_mut();
        if dark {
            s.display.theme = "dark".into();
        }
        s.networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 16667, false), Server::new("localhost", 16697, true)]));
        s.address_book.notify.push(NotifyEntry { nick: "Elisa".into(), note: "de Lyon".into(), network: None });
        s.address_book.notify.push(NotifyEntry { nick: "Kaelis".into(), note: String::new(), network: Some("Undernet".into()) });
        s.address_book.ignore.push(IgnoreEntry {
            mask: "*!*@spam.example".into(),
            private: true,
            channel: true,
            notice: true,
            ctcp: true,
            invite: true,
            dcc: true,
            ..Default::default()
        });
        s.address_book.control.push(ControlEntry {
            kind: ControlKind::Autoop,
            mask: "Nemo!*@grenoble.fr".into(),
            channels: vec!["#lyon".into()],
            network: None,
        });
        s.highlight.words.push(alpirc_core::settings::HighlightWord { text: "alpirc".into(), color: Some(4), beep: false, flash: true, notify: true });
    }
    shell.settings_changed(&mut p);
    shell.set_viewport(Size::new(w, h));
    shell.start(&mut p);
    // Une connexion simulée, un canal où l'on est op, quelques bans.
    let status = shell.client.windows()[0].id;
    shell.client.connect(status, "Undernet", None, p.now);
    shell.tick(&mut p);
    let conn = shell.client.conns()[0];
    shell.client.on_connected(conn, p.now);
    for l in [
        ":irc.undernet.org 001 Guiwald :Welcome to the UnderNet IRC Network, Guiwald",
        ":irc.undernet.org 005 Guiwald NETWORK=UnderNet PREFIX=(ov)@+ CHANMODES=b,k,l,imnpstrDd MODES=6 :are supported by this server",
        ":irc.undernet.org 376 Guiwald :End of /MOTD command.",
        ":Guiwald!~alpirc@lyon.wanadoo.fr JOIN #lyon",
        ":irc.undernet.org 332 Guiwald #lyon :Bienvenue sur #lyon ! Soirée IRL vendredi place des Terreaux",
        ":irc.undernet.org 333 Guiwald #lyon Kaelis!kae@lyon.fr 1792250000",
        ":irc.undernet.org 353 Guiwald = #lyon :@Guiwald @Kaelis +Elisa Bibi Nemo",
        ":irc.undernet.org 366 Guiwald #lyon :End of /NAMES list.",
        ":irc.undernet.org 324 Guiwald #lyon +ntl 42",
        ":irc.undernet.org 367 Guiwald #lyon *!*@spam.example Kaelis!kae@lyon.fr 1792250000",
        ":irc.undernet.org 367 Guiwald #lyon Troll!*@* Guiwald!~alpirc@lyon.wanadoo.fr 1792255000",
        ":irc.undernet.org 368 Guiwald #lyon :End of Channel Ban List",
        ":Elisa!~elisa@lyon.fr PRIVMSG #lyon :coucou !",
    ] {
        p.now += 1000;
        shell.on_net(NetEvent::Line { conn, bytes: l.as_bytes().to_vec() }, &mut p);
    }
    let chan = shell.client.find_window(Some(conn), "#lyon").unwrap();

    // Options : chaque page.
    for page in ["connect", "servers", "options", "irc", "highlight", "messages", "flood", "logging", "catcher", "display", "notifications", "scripts", "dcc"] {
        shell.open_ui_dialog(UiDialog::Options(Some(page.into())), &mut p);
        shot(&mut shell, &out, &format!("options-{page}"));
        close(&mut shell, &mut p);
    }
    // Éditeur de réseau (depuis Options › Serveurs : Entrée sur le premier réseau = Modifier).
    let net = shell.client.settings().network("Undernet").unwrap().clone();
    let mut net = net;
    net.servers.push(Server {
        host: "eu.undernet.org".into(),
        port: 6697,
        tls: true,
        verify: alpirc_core::settings::Verify::Pinned,
        fingerprint: Some("ab".repeat(32)),
        ..Server::default()
    });
    net.undernet_x = Some(alpirc_core::settings::UndernetX { username: "Guiwald".into(), ..Default::default() });
    net.perform = vec!["/mode $me +x".into(), "/join #lyon".into()];
    shell.open_dialog(Box::new(alpirc_shell::dialogs::network::NetworkEditor::new(net, false, Box::new(|_, _| Ok(())))), &mut p);
    shots_with_tabs(&mut shell, &mut p, &out, "network", &["servers", "identity", "login", "perform"]);
    // Choix du serveur.
    shell.open_ui_dialog(UiDialog::Connect, &mut p);
    shot(&mut shell, &out, "connect");
    close(&mut shell, &mut p);
    // Carnet d'adresses.
    shell.open_ui_dialog(UiDialog::AddressBook, &mut p);
    shots_with_tabs(&mut shell, &mut p, &out, "abook", &["notify", "ignore", "control"]);
    // Dossier des canaux.
    shell.open_ui_dialog(UiDialog::ChannelsFolder, &mut p);
    shot(&mut shell, &out, "folder");
    close(&mut shell, &mut p);
    // Channel Central.
    shell.open_ui_dialog(UiDialog::ChannelCentral(chan), &mut p);
    shots_with_tabs(&mut shell, &mut p, &out, "central", &["bans", "excepts", "invites"]);
    // Éditeur de scripts.
    shell.open_ui_dialog(UiDialog::ScriptEditor, &mut p);
    shot(&mut shell, &out, "scripts-remote");
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    shot(&mut shell, &out, "scripts-aliases");
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    shot(&mut shell, &out, "scripts-popups");
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    key(&mut shell, &mut p, Key::Tab, Mods::CTRL);
    shot(&mut shell, &out, "scripts-variables");
    close(&mut shell, &mut p);
    // Couleurs.
    shell.open_ui_dialog(UiDialog::Colors, &mut p);
    shot(&mut shell, &out, "colors");
    close(&mut shell, &mut p);
    // Certificat refusé.
    shell.on_net(NetEvent::Failed { conn, error: "certificate signed by an unknown authority".into(), cert_sha256: Some("3f2a".repeat(16)) }, &mut p);
    shot(&mut shell, &out, "cert");
    close(&mut shell, &mut p);
}
