//! Réglages (TOML, valeurs par défaut, fichier illisible, droits), thèmes et couleurs, langues, journaux.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use alpirc_core::i18n::{self, Key, Lang};
use alpirc_core::settings::{self, ControlEntry, ControlKind, IgnoreEntry, NotifyEntry, SaslSettings, UndernetX};
use alpirc_core::{Effect, LineKind, Settings};
use common::*;

#[test]
fn defaults() {
    let s = Settings::default();
    assert_eq!(s.version, 1);
    let names: Vec<&str> = s.networks.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["Undernet", "Libera.Chat", "OFTC", "IRCnet", "EFnet", "DALnet", "QuakeNet", "Rizon", "EpiKnet"]);
    let u = &s.networks[0].servers[0];
    assert_eq!((u.host.as_str(), u.port, u.tls), ("irc.undernet.org", 6667, false));
    let l = &s.networks[1].servers[0];
    assert_eq!((l.host.as_str(), l.port, l.tls), ("irc.libera.chat", 6697, true));
    let favs: Vec<(&str, &str, bool)> = s.favorites.iter().map(|f| (f.channel.as_str(), f.network.as_str(), f.join_on_connect)).collect();
    assert_eq!(favs, vec![("#lyon", "Undernet", false), ("#grenoble", "Undernet", false), ("#chambery", "Undernet", false)]);
    assert_eq!(s.identity.nick, "Guiwald");
    assert_eq!(s.identity.alt_nick, "Guiwald_");
    assert_eq!(s.identity.realname, "AlpIRC user");
    assert_eq!(s.identity.username, "alpirc");
    assert_eq!(s.identity.quit_message, "AlpIRC — https://github.com/Guiwald/alpirc");
    assert!(s.irc.timestamps);
    assert_eq!(s.irc.timestamp_format, "[HH:nn]");
    assert_eq!(s.irc.max_lines, 5000);
    assert_eq!(s.irc.confirm_paste_lines, 2);
    assert!(s.connect.reconnect && s.connect.rejoin_channels);
    assert_eq!(s.connect.retry_delay_secs, 10);
    assert_eq!((s.flood.burst, s.flood.rate_ms, s.flood.ctcp_flood_limit), (5, 2000, 3));
    assert_eq!(s.away.default_message, "Away");
    assert_eq!(s.catcher.max, 500);
    assert_eq!(s.display.chat_font, "Fixedsys Excelsior");
    assert_eq!(s.display.switchbar, "top");
    assert!(!s.logging.enabled);
    assert!(s.notifications.only_when_inactive);
}

#[test]
fn default_colors() {
    let s = Settings::default();
    let expect = [
        (LineKind::Action, 6),
        (LineKind::Ctcp, 4),
        (LineKind::Highlight, 4),
        (LineKind::Info, 2),
        (LineKind::Info2, 12),
        (LineKind::Invite, 3),
        (LineKind::Join, 3),
        (LineKind::Kick, 3),
        (LineKind::Mode, 3),
        (LineKind::Nick, 3),
        (LineKind::Normal, 1),
        (LineKind::Notice, 5),
        (LineKind::Notify, 4),
        (LineKind::Other, 6),
        (LineKind::Own, 1),
        (LineKind::Part, 3),
        (LineKind::Quit, 2),
        (LineKind::Topic, 3),
        (LineKind::Wallops, 5),
        (LineKind::Whois, 1),
        (LineKind::Error, 4),
    ];
    for (k, c) in expect {
        assert_eq!(s.color_index(k), c, "{k:?}");
    }
    assert_eq!(s.display.colors.background, 0);
    assert_eq!(s.display.palette, settings::MIRC_PALETTE);
    let h = H::new();
    assert_eq!(h.c.color_index(LineKind::Notice), 5);
}

#[test]
fn themes() {
    let t = settings::themes();
    assert_eq!(t.iter().map(|t| t.name).collect::<Vec<_>>(), vec!["classic", "dark"]);
    // Sombre : la palette de l'interface (blanc et noir échangés), mêmes numéros que le classique, texte lisible.
    let dark = settings::theme("Dark").unwrap();
    assert_eq!(dark.palette, settings::DARK_PALETTE);
    assert_eq!(dark.colors, settings::EventColors::default());
    let lum = |c: u32| ((c >> 16) & 0xFF) + ((c >> 8) & 0xFF) + (c & 0xFF);
    assert!(lum(dark.palette[dark.colors.normal as usize]) > lum(dark.palette[dark.colors.background as usize]) + 300);
    assert_eq!(settings::theme("classic").unwrap().colors, settings::EventColors::default());
    assert_eq!(settings::theme("sombre").unwrap().name, "dark");
    assert!(settings::is_builtin_theme("Classique") && !settings::is_builtin_theme("Nuit"));
}

#[test]
fn user_themes_save_select_delete() {
    use settings::{ThemeNameError, UserTheme};
    let mut d = Settings::default().display;
    assert_eq!(d.theme_names(), vec!["classic", "dark"]);
    let mut night = UserTheme { name: " Nuit ".into(), base: "dark".into(), palette: settings::DARK_PALETTE, ..UserTheme::default() };
    night.palette[0] = 0x000020;
    night.colors.action = 9;
    assert_eq!(d.store_theme(night.clone()), Ok("Nuit".to_string()));
    assert_eq!(d.store_theme(UserTheme { name: "  ".into(), ..UserTheme::default() }), Err(ThemeNameError::Empty));
    assert_eq!(d.store_theme(UserTheme { name: "Sombre".into(), ..UserTheme::default() }), Err(ThemeNameError::Builtin));
    // Même nom (casse indifférente) : remplacé.
    night.colors.action = 11;
    d.store_theme(UserTheme { name: "nuit".into(), ..night.clone() }).unwrap();
    assert_eq!(d.user_themes.len(), 1);
    assert_eq!(d.user_theme("NUIT").unwrap().colors.action, 11);
    assert_eq!(d.theme_names(), vec!["classic", "dark", "nuit"]);
    // Choisir : nom, palette et couleurs ; l'allure des fenêtres vient de la base.
    assert!(d.select_theme("Nuit"));
    assert_eq!((d.theme.as_str(), d.palette[0], d.colors.action), ("nuit", 0x000020, 11));
    assert_eq!(d.theme_base(), "dark");
    assert_eq!(d.effective_colors().0[0], 0x000020);
    assert!(!d.select_theme("absent"));
    assert_eq!(d.theme, "nuit");
    // Supprimer le thème choisi : le choix revient à sa base, les couleurs restent.
    assert!(d.delete_theme("NUIT"));
    assert_eq!((d.theme.as_str(), d.palette[0]), ("dark", 0x000020));
    assert!(d.user_themes.is_empty() && !d.delete_theme("Nuit"));
    assert!(d.select_theme("classic"));
    assert_eq!((d.palette, d.colors.clone()), (settings::MIRC_PALETTE, settings::EventColors::default()));
}

#[test]
fn effective_colors_follow_builtin_themes() {
    let mut d = Settings::default().display;
    // Anciens réglages « sombres » (menu) : la palette classique est restée ; on montre celle du thème sombre.
    d.theme = "dark".into();
    assert_eq!(d.effective_colors().0, settings::DARK_PALETTE);
    assert_eq!(d.theme_base(), "dark");
    // Retour au classique par le menu avec les couleurs du sombre : celles du classique.
    d.select_theme("dark");
    d.theme = "classic".into();
    assert_eq!(d.effective_colors().0, settings::MIRC_PALETTE);
    // Couleurs retouchées : elles sont gardées telles quelles.
    d.palette[4] = 0xFF6600;
    assert_eq!(d.effective_colors().0[4], 0xFF6600);
    // Un thème inconnu (supprimé à la main du fichier) : allure classique.
    d.theme = "perdu".into();
    assert_eq!(d.theme_base(), "classic");
}

#[test]
fn old_settings_files_still_load() {
    // Fichier d'avant les sons, les thèmes nommés et le choix de la police : valeurs par défaut pour le reste.
    let s = Settings::from_toml(
        "[display]\ntheme = \"dark\"\nchat_font = \"Fixedsys Excelsior\"\nchat_font_size = 32\npalette = [16777215, 0, 127, 37632, \
         16711680, 8323072, 10223772, 16547584, 16776960, 64512, 37779, 65535, 252, 16711935, 8355711, 13816530]\n\n\
         [display.colors]\naction = 13\n",
    )
    .unwrap();
    assert_eq!(s.display.theme, "dark");
    assert_eq!(s.display.chat_font_size, 32);
    assert_eq!(s.display.colors.action, 13);
    assert!(s.display.user_themes.is_empty());
    assert_eq!(s.sounds, settings::SoundOptions::default());
    // Thèmes et sons survivent à l'aller-retour ; l'aperçu de police n'est jamais écrit.
    let mut s = Settings::default();
    s.display.store_theme(settings::UserTheme { name: "Perso".into(), ..Default::default() }).unwrap();
    s.sounds.notify.play = settings::SoundPlay::Beep;
    s.sounds.mute = true;
    s.display.font_preview = Some(("DejaVu Sans Mono".into(), 13));
    let text = s.to_toml().unwrap();
    assert!(text.contains("[sounds.notify]") && text.contains("play = \"beep\""), "{text}");
    assert!(!text.contains("DejaVu"));
    let back = Settings::from_toml(&text).unwrap();
    s.display.font_preview = None;
    assert_eq!(back, s);
}

#[test]
fn sound_defaults_and_paths() {
    use settings::{SoundEvent, SoundPlay, sound_path};
    let s = settings::SoundOptions::default();
    assert!(!s.mute && !s.accept_requests);
    // Seuls « on vous nomme » et les privés sonnent ; chaque événement a son son fourni.
    for e in SoundEvent::ALL {
        let on = matches!(e, SoundEvent::Highlight | SoundEvent::Query);
        assert_eq!(s.get(e).play == SoundPlay::File, on, "{e:?}");
        assert_eq!(s.get(e).file, e.default_file());
        assert!(settings::BUILTIN_SOUNDS.contains(&e.default_file()));
    }
    let d = Path::new("/donnees");
    assert_eq!(sound_path(d, "ding.wav").as_deref(), Some(Path::new("/donnees/sounds/ding.wav")));
    assert_eq!(sound_path(d, "/ailleurs/x.wav").as_deref(), Some(Path::new("/ailleurs/x.wav")));
    assert_eq!(sound_path(d, "../x.wav"), None);
    assert_eq!(sound_path(d, " "), None);
}

fn rich() -> Settings {
    let mut s = Settings::default();
    s.language = Some("fr".into());
    let n = s.network_mut("Undernet").unwrap();
    n.undernet_x = Some(UndernetX { username: "gui".into(), password: "pw".into(), hide_host: true, require_x: true });
    n.perform = vec!["join #lyon".into()];
    n.encoding = "latin1".into();
    let l = s.network_mut("Libera.Chat").unwrap();
    l.sasl = Some(SaslSettings { mechanism: "SCRAM-SHA-256".into(), account: "gui".into(), password: "x".into(), client_cert: None });
    l.servers[0].verify = settings::Verify::Pinned;
    l.servers[0].fingerprint = Some("ab:cd".into());
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), note: "ami".into(), network: None });
    s.address_book.ignore.push(IgnoreEntry { mask: "*!*@spam".into(), private: true, until_ms: Some(42), ..IgnoreEntry::default() });
    s.address_book.control.push(ControlEntry { kind: ControlKind::Protect, mask: "Pat".into(), channels: vec!["#lyon".into()], network: None });
    s.display.colors.action = 13;
    s.display.palette[0] = 0x123456;
    s
}

#[test]
fn toml_roundtrip() {
    let s = rich();
    let text = s.to_toml().unwrap();
    assert!(text.contains("version = 1"), "{text}");
    let back = Settings::from_toml(&text).unwrap();
    assert_eq!(back, s);
}

#[test]
fn partial_file_keeps_defaults() {
    let s = Settings::from_toml("[identity]\nnick = \"Toto\"\n\n[irc]\ntimestamps = false\n").unwrap();
    assert_eq!(s.identity.nick, "Toto");
    assert_eq!(s.identity.alt_nick, "Guiwald_");
    assert!(!s.irc.timestamps);
    assert!(s.irc.show_quits);
    assert_eq!(s.networks.len(), 9);
    assert_eq!(s.version, 1);
}

#[test]
fn load_and_save_files() {
    let dir = tmpdir();
    let path = dir.join("alpirc.toml");
    assert_eq!(Settings::load(&path), Settings::default(), "absent : défauts");
    let s = rich();
    s.save(&path).unwrap();
    assert_eq!(Settings::load(&path), s);
    assert!(!dir.join("alpirc.toml.tmp").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    // Fichier illisible : copie de secours et défauts.
    std::fs::write(&path, "this is = = not toml [").unwrap();
    assert_eq!(Settings::load(&path), Settings::default());
    assert_eq!(std::fs::read_to_string(dir.join("alpirc.toml.bak")).unwrap(), "this is = = not toml [");
}

#[test]
fn settings_changed_reapplies_and_requests_save() {
    let mut h = H::new();
    h.c.settings_mut().language = Some("fr".into());
    h.c.settings_changed(h.now);
    assert_eq!(h.c.language(), Lang::Fr);
    assert!(h.effects().contains(&Effect::SaveSettings));
    assert_eq!(h.c.window(h.status).unwrap().title, "Status : Guiwald (non connecté)");
}

#[test]
fn french_output() {
    let mut s = settings();
    s.language = Some("fr".into());
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat");
    assert_eq!(h.lines(w)[0], "* Vous parlez maintenant dans #lyon");
    h.feed(":Bob!b@h JOIN #lyon");
    assert_eq!(h.last(w), "* Arrivée : Bob (b@h)");
    h.feed(":Pat!p@h KICK #lyon Bob :dehors");
    assert_eq!(h.last(w), "* Bob a été expulsé par Pat (dehors)");
    h.feed(":Pat!p@h TOPIC #lyon :Sujet");
    assert_eq!(h.last(w), "* Pat change le sujet en « Sujet »");
    h.input(h.status, "salut");
    assert_eq!(h.last(h.status), "* Vous n'êtes pas sur un canal");
}

#[test]
fn system_locale_used_without_setting() {
    let mut s = settings();
    s.language = None;
    let mut h = H::with(s);
    assert_eq!(h.c.language(), Lang::En);
    h.c.set_system_locale("fr_FR.UTF-8");
    assert_eq!(h.c.language(), Lang::Fr);
    h.c.set_system_locale("ga_IE.UTF-8");
    assert_eq!(h.c.language(), Lang::Ga);
    // Les textes du moteur sont alors en irlandais.
    h.input(h.status, "salut");
    assert_eq!(h.last(h.status), "* Níl tú ar chainéal");
}

#[test]
fn i18n_coverage_listing() {
    // Rapport de couverture (ne fait pas échouer les essais) : anglais et français complets.
    let cov = i18n::coverage();
    for (l, n, total) in &cov {
        println!("{:>2} {:<9} {n}/{total}", l.code(), l.native_name());
    }
    assert_eq!(cov.len(), 7);
    assert!(cov.iter().all(|(_, _, t)| *t == Key::ALL.len()));
    assert_eq!(cov[0].1, cov[0].2);
    assert_eq!(cov[1].1, cov[1].2);
}

#[test]
fn logging_to_files() {
    let mut s = settings();
    s.logging.enabled = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#Lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #Lyon :\x02salut\x02");
    h.feed(":Pat!p@h PRIVMSG Guiwald :privé");
    let chan_log = h.dir.join("logs/testnet/#lyon.log");
    let text = std::fs::read_to_string(&chan_log).unwrap();
    assert!(text.contains("Session Start: Thu Oct 09 08:53:20 2025"), "{text}");
    assert!(text.contains("[08:53] <Pat> salut\n"), "codes retirés, horodaté : {text}");
    let q = std::fs::read_to_string(h.dir.join("logs/testnet/pat.log")).unwrap();
    assert!(q.contains("<Pat> privé"));
    assert!(!h.dir.join("logs/testnet/status.log").exists(), "Status non journalisé par défaut");
    h.c.close_window(h.win("Pat"), h.now);
    let q = std::fs::read_to_string(h.dir.join("logs/testnet/pat.log")).unwrap();
    assert!(q.ends_with("Session Close: Thu Oct 09 08:53:20 2025\n"), "{q}");
    // /log off : plus rien pour cette fenêtre.
    h.input(w, "/log off");
    h.feed(":Pat!p@h PRIVMSG #Lyon :non journalisé");
    let text = std::fs::read_to_string(&chan_log).unwrap();
    assert!(!text.contains("non journalisé"));
}

struct Memory(Arc<Mutex<Vec<(String, String)>>>);

impl alpirc_core::client::LogSink for Memory {
    fn append(&mut self, path: &Path, text: &str) -> std::io::Result<()> {
        self.0.lock().unwrap().push((path.display().to_string(), text.to_string()));
        Ok(())
    }
}

#[test]
fn custom_log_sink_and_folder() {
    let mut s = settings();
    s.logging.enabled = true;
    s.logging.timestamps = false;
    s.logging.strip_codes = false;
    s.logging.folder = Some("/journaux".into());
    let mut h = H::with(s);
    let mem = Arc::new(Mutex::new(Vec::new()));
    h.c.set_log_sink(Box::new(Memory(mem.clone())));
    h.up();
    h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :\x02gras\x02");
    let v = mem.lock().unwrap().clone();
    assert!(v.iter().all(|(p, _)| p == "/journaux/testnet/#lyon.log"));
    assert!(v.last().unwrap().1.ends_with("<Pat> \x02gras\x02\n"));
    assert!(v[0].1.starts_with("\nSession Start:"));
    assert!(!h.dir.join("logs").exists());
}

#[test]
fn log_reload_lines() {
    let dir = tmpdir();
    std::fs::create_dir_all(dir.join("logs/testnet")).unwrap();
    std::fs::write(dir.join("logs/testnet/#lyon.log"), "\nSession Start: x\n[08:00] <Pat> ancien 1\n[08:01] <Pat> ancien 2\nSession Close: x\n").unwrap();
    let mut s = settings();
    s.logging.enabled = true;
    s.logging.reload_lines = 1;
    let mut h = H::with_dir(s, dir);
    h.up();
    let w = h.join("#lyon", "Guiwald");
    let l = h.c.window(w).unwrap().lines.clone();
    assert_eq!(l[0].text, "[08:01] <Pat> ancien 2");
    assert!(l[0].from_history);
    assert_eq!(l[1].text, "* Now talking in #lyon");
}
